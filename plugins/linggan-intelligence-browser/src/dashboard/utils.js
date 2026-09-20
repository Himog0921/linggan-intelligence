import { generateCsv, downloadFile, getUnifiedAuthorHandle } from '../shared/utils.js';
import { unwrapCompatResponseData } from '../shared/responseEnvelope.js';

export { generateCsv, downloadFile, getUnifiedAuthorHandle };

export function extractHashtags(text = '') {
  const raw = String(text || '');
  const matches = raw.match(/#\s*[^\s#，。！？,.!?:：;；、]+/g) || [];
  return matches.map((tag) => tag.replace(/^#/, '').trim()).filter(Boolean);
}

export function stripHashtags(text = '') {
  return String(text || '')
    .replace(/#\s*[^\s#，。！？,.!?:：;；、]+/g, '')
    .replace(/\s+/g, ' ')
    .trim();
}

export function stripLooseHashes(text = '') {
  return String(text || '')
    .replace(/^\s*#{1,6}\s*/gm, '')
    .replace(/(^|\s)(?:#\s*){2,}(?=\s|$)/g, ' ')
    .replace(/\s+/g, ' ')
    .trim();
}

export function cleanDisplayBodyText(text = '') {
  return stripLooseHashes(stripHashtags(text));
}

export function getHashtagsForItem(item = {}) {
  const base = Array.isArray(item.hashtags)
    ? item.hashtags
    : (typeof item.hashtags === 'string' ? item.hashtags.split(/[,\n;]/) : []);
  const fromTitle = extractHashtags(item.title || '');
  const fromContent = extractHashtags(item.content || '');
  return [...new Set([...base, ...fromTitle, ...fromContent].map((v) => String(v || '').trim()).filter(Boolean))];
}

export function formatReplyTargetLabel(item = {}) {
  const level = Number(item.level || 0);
  const target = String(item.replyToUserName || '').trim();
  if (level <= 1) return '主评论';
  if (target) return target;
  return '回复主评论';
}

export function formatCollectionRunLabel(runId = '') {
  const text = String(runId || '').trim();
  if (!text) return '-';
  const prefix = text.split('_')[0] || text;
  const labelMap = {
    batchComments: '批量评论',
    batchNotes: '批量视频',
    singleComments: '单条评论',
    commentImageDownload: '评论图片区',
    singleNotes: '单条内容',
  };
  const suffix = text.slice(-6);
  return `${labelMap[prefix] || prefix} · ${suffix}`;
}

export function formatBatchSelectionModeLabel(value = '') {
  const mode = String(value || '').trim();
  if (mode === 'top_likes') return '点赞 Top N';
  if (mode === 'profile_order') return '主页顺位';
  if (mode === 'search_order') return '搜索顺位';
  if (mode === 'search_top_likes') return '搜索点赞 Top N';
  return '-';
}

export function formatDataQualityLabel(value = '') {
  const quality = String(value || '').trim();
  const labelMap = {
    full: '完整',
    degraded: '降级',
    seed: '种子',
  };
  return labelMap[quality] || (quality ? quality : '-');
}

export function formatSourceTierLabel(value = '') {
  const tier = String(value || '').trim();
  const labelMap = {
    api: 'API',
    render: '渲染态',
    mixed: '混合',
    dom: 'DOM',
    seed: '种子',
  };
  return labelMap[tier] || (tier ? tier : '-');
}

export function formatQualityReasonLabel(value = '') {
  const reason = String(value || '').trim();
  const labelMap = {
    api_unobserved_dom_fallback: 'API 未命中，回退 DOM',
    api_snapshot_partial: 'API 快照不完整',
    synthetic_comment_id: '合成评论 ID',
    search_summary_seed: '搜索摘要种子',
    aweme_seed_without_detail: 'Aweme 种子未补全',
    detail_context_dom_fallback: '详情上下文回退 DOM',
    inject_failed_dom_only: '注入失败，仅 DOM',
    handle_route_fallback: '路由句柄回退',
    monitor_surface_seed: '监控 surface 种子',
    dom_marker_ip_fallback: 'IP 回退 DOM 标记',
    render_user_mismatch: '渲染态用户不匹配',
    surface_card_only: 'Surface 卡片种子',
  };
  if (labelMap[reason]) return labelMap[reason];
  if (!reason) return '-';
  return reason.replace(/_/g, ' ');
}

export function truncate(str, max) {
  const s = String(str || '');
  return s.length > max ? s.slice(0, max) + '...' : s;
}

export function escapeHtml(str) {
  return String(str || '')
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;')
    .replace(/'/g, '&#39;');
}

export function debounce(fn, delay) {
  let timer;
  return (...args) => {
    clearTimeout(timer);
    timer = setTimeout(() => fn(...args), delay);
  };
}

export function sortByCreatedAt(data, sort = 'desc') {
  return [...data].sort((a, b) => {
    const ta = a.createdAt || 0;
    const tb = b.createdAt || 0;
    return sort === 'asc' ? ta - tb : tb - ta;
  });
}

export function formatLocalDate(timestamp) {
  if (!timestamp) return '';
  const d = new Date(timestamp);
  const y = d.getFullYear();
  const m = String(d.getMonth() + 1).padStart(2, '0');
  const day = String(d.getDate()).padStart(2, '0');
  return `${y}-${m}-${day}`;
}

export function toDisplayUrl(url) {
  const normalized = normalizeUrl(url);
  return normalized || '-';
}

export function normalizeUrl(url = '') {
  const val = String(url || '').trim();
  if (!val || val === 'undefined' || val === 'null') return '';
  if (val.startsWith('http')) return val;
  if (val.startsWith('//')) return `https:${val}`;
  if (val.startsWith('/')) return `https://www.xiaohongshu.com${val}`;
  return val;
}

function isXhsShareUrl(url = '') {
  const normalized = normalizeUrl(url);
  if (!normalized) return false;
  return /xsec_token=/i.test(normalized) || /xhslink\.com/i.test(normalized);
}

export function getPreferredRecordUrl(item = {}, key = '') {
  const directUrl = normalizeUrl(item?.[key]);
  if (key !== 'noteUrl') return directUrl;

  const platformHint = String(item?.platform || '').trim().toLowerCase();
  const platform = platformHint || (/douyin\.com/i.test(`${item?.noteUrl || ''} ${item?.rawUrl || ''}`) ? 'douyin' : 'xhs');
  if (platform !== 'xhs') return directUrl;

  const rawUrl = normalizeUrl(item?.rawUrl);
  if (isXhsShareUrl(directUrl)) return directUrl;
  if (isXhsShareUrl(rawUrl)) return rawUrl;
  return directUrl || rawUrl || '';
}

export function getItemId(item, tab) {
  if (tab === 'notes') return String(item.noteId || '').trim();
  if (tab === 'comments') return item.id != null ? item.id : String(item.commentId || '').trim();
  return String(item.userId || '').trim();
}

export function getTabLabel(tab) {
  const labels = { notes: '笔记', comments: '评论', authors: '博主' };
  return labels[tab] || tab;
}

export function getColumns(tab, allData) {
  switch (tab) {
    case 'notes': {
      const platforms = new Set(allData.map(d => d.platform || 'xhs'));
      const isAllDouyin = platforms.size === 1 && platforms.has('douyin');
      const isAllXhs = platforms.size === 1 && !platforms.has('douyin');
      const commonCols = [
        { key: 'noteId', label: 'ID' },
        { key: 'title', label: '标题' },
        { key: 'hashtags', label: '话题' },
        { key: 'mediaPreview', label: '媒体预览' },
        { key: 'mediaDownloadStatus', label: '下载状态' },
        { key: 'dataQuality', label: '质量' },
        { key: 'qualityReason', label: '质量原因' },
        { key: 'sourceTier', label: '来源层级' },
        { key: 'type', label: '类型' },
        { key: 'authorName', label: '作者' },
        { key: 'ipLocation', label: 'IP属地' },
        { key: 'likes', label: '点赞' },
        { key: 'collects', label: '收藏' },
        { key: 'comments', label: '评论数' },
        { key: 'url', label: '链接', className: 'col-note-link' },
        { key: 'createdAt', label: '采集时间' },
      ];
      if (isAllDouyin) {
        return [...commonCols,
          { key: 'searchKeyword', label: '搜索词' },
          { key: 'batchSelectionMode', label: '入选方式' },
          { key: 'batchRank', label: '批量序位' },
          { key: 'batchLikesSnapshot', label: '入选点赞' },
          { key: 'shares', label: '转发数' },
          { key: 'playCount', label: '播放量' },
        ];
      }
      if (isAllXhs) {
        return [...commonCols,
          { key: 'content', label: '正文' },
          { key: 'images', label: '图片数' },
          { key: 'topicIds', label: '话题数' },
          { key: 'lastUpdateTime', label: '最后修改时间' },
        ];
      }
      return [{ key: 'platform', label: '平台' }, ...commonCols];
    }
    case 'comments':
      return [
        { key: 'platform', label: '平台', className: 'col-comment-platform' },
        { key: 'author', label: '作者', className: 'col-comment-author' },
        { key: 'text', label: '内容', className: 'col-comment-text' },
        { key: 'replyToUserName', label: '回复对象', className: 'col-comment-reply' },
        { key: 'likes', label: '点赞', className: 'col-comment-count' },
        { key: 'level', label: '层级', className: 'col-comment-level' },
        { key: 'ipLocation', label: 'IP属地', className: 'col-comment-location' },
        { key: 'noteUrl', label: '作品', className: 'col-comment-link' },
        { key: 'commentId', label: '评论ID', className: 'col-comment-id' },
        { key: 'collectionRunId', label: '采集批次', className: 'col-comment-batch' },
        { key: 'dataQuality', label: '质量', className: 'col-comment-quality' },
        { key: 'qualityReason', label: '质量原因', className: 'col-comment-quality-reason' },
        { key: 'sourceTier', label: '来源层级', className: 'col-comment-source-tier' },
        { key: 'createdAt', label: '采集时间', className: 'col-comment-time' },
      ];
    case 'authors': {
      const platforms = new Set(allData.map(d => d.platform || 'xhs'));
      const isAllDouyin = platforms.size === 1 && platforms.has('douyin');
      const isAllXhs = platforms.size === 1 && !platforms.has('douyin');
      const accountIdLabel = isAllDouyin ? '抖音号' : (isAllXhs ? '小红书号' : '平台账号');
      return [
        { key: 'avatar', label: '头像' },
        { key: 'userId', label: 'ID' },
        { key: 'name', label: '昵称' },
        { key: 'handle', label: accountIdLabel },
        { key: 'profileUrl', label: '主页链接', className: 'col-author-link' },
        { key: 'fans', label: '粉丝' },
        { key: 'interactions', label: '获赞' },
        { key: 'ipLocation', label: 'IP属地' },
        { key: 'accountStatus', label: '账号状态' },
        { key: 'followedByMe', label: '我已关注' },
        { key: 'dataQuality', label: '质量' },
        { key: 'qualityReason', label: '质量原因' },
        { key: 'sourceTier', label: '来源层级' },
        { key: 'createdAt', label: '采集时间' },
      ];
    }
    default:
      return [];
  }
}

export function getExportColumns(tab, allData) {
  switch (tab) {
    case 'notes': {
      const platforms = new Set(allData.map(d => d.platform || 'xhs'));
      const isAllDouyin = platforms.size === 1 && platforms.has('douyin');
      const base = [
        { key: 'noteId', label: 'ID' },
        { key: 'title', label: '标题' },
        { key: 'content', label: '正文' },
        { key: 'hashtags', label: '话题' },
        { key: 'type', label: '类型' },
        { key: 'authorName', label: '作者' },
        { key: 'authorId', label: '作者ID' },
        { key: 'likes', label: '点赞' },
        { key: 'collects', label: '收藏' },
        { key: 'comments', label: '评论数' },
        { key: 'shares', label: '分享' },
        { key: 'url', label: '链接' },
        { key: 'dataQuality', label: '质量' },
        { key: 'qualityReason', label: '质量原因' },
        { key: 'sourceTier', label: '来源层级' },
        { key: 'createdAt', label: '采集时间' },
      ];
      if (isAllDouyin) {
        return [...base,
          { key: 'searchKeyword', label: '搜索词' },
          { key: 'batchSelectionMode', label: '入选方式' },
          { key: 'batchRank', label: '批量序位' },
          { key: 'batchLikesSnapshot', label: '入选点赞' },
          { key: 'playCount', label: '播放量' },
        ];
      }
      return [...base,
        { key: 'images', label: '图片' },
        { key: 'topicIds', label: '话题数' },
      ];
    }
    case 'comments':
      return [
        { key: 'commentId', label: 'ID' },
        { key: 'contentId', label: '作品ID' },
        { key: 'text', label: '内容' },
        { key: 'author', label: '作者' },
        { key: 'authorId', label: '作者ID' },
        { key: 'replyToUserName', label: '回复对象' },
        { key: 'likes', label: '点赞' },
        { key: 'level', label: '层级' },
        { key: 'rootCommentId', label: '根评论ID' },
        { key: 'parentCommentId', label: '父评论ID' },
        { key: 'dataQuality', label: '质量' },
        { key: 'qualityReason', label: '质量原因' },
        { key: 'sourceTier', label: '来源层级' },
        { key: 'url', label: '链接' },
        { key: 'createdAt', label: '采集时间' },
      ];
    case 'authors':
      return [
        { key: 'userId', label: 'ID' },
        { key: 'name', label: '昵称' },
        { key: 'handle', label: '平台账号' },
        { key: 'description', label: '简介' },
        { key: 'fans', label: '粉丝' },
        { key: 'follows', label: '关注' },
        { key: 'interactions', label: '获赞' },
        { key: 'ipLocation', label: 'IP属地' },
        { key: 'profileUrl', label: '主页链接' },
        { key: 'dataQuality', label: '质量' },
        { key: 'qualityReason', label: '质量原因' },
        { key: 'sourceTier', label: '来源层级' },
        { key: 'createdAt', label: '采集时间' },
      ];
    default:
      return [];
  }
}

export async function sendToParent(action, data = {}, options = {}) {
  let nonce = readDashboardNonceFromUrl();
  if (!nonce) {
    nonce = await readDashboardNonceFromStorage();
  }

  return new Promise((resolve) => {
    const timeoutMs = Number(options.timeoutMs ?? 3000);
    let settled = false;
    const channel = new MessageChannel();
    const settle = (value) => {
      if (settled) return;
      settled = true;
      try { channel.port1.close?.(); } catch {}
      resolve(value);
    };
    channel.port1.onmessage = (e) => {
      settle(e.data);
    };
    window.parent.postMessage({ source: 'lgboom-dashboard', action, nonce, ...data }, '*', [channel.port2]);
    if (timeoutMs > 0) {
      setTimeout(() => {
        settle(null);
      }, timeoutMs);
    }
  });
}

async function readDashboardNonceFromStorage() {
  const areas = [
    globalThis.chrome?.storage?.session,
    globalThis.chrome?.storage?.local,
  ].filter(Boolean);

  for (const area of areas) {
    try {
      const result = await area.get(['dashboardNonce']);
      const nonce = String(result?.dashboardNonce || '').trim();
      if (nonce) return nonce;
    } catch (e) {
      console.error('[Dashboard] Failed to read nonce:', e);
    }
  }

  return '';
}

function readDashboardNonceFromUrl() {
  try {
    const href = window?.location?.href || '';
    const search = window?.location?.search || '';
    const hash = String(window?.location?.hash || '').replace(/^#/, '');
    const params = href
      ? new URL(href).searchParams
      : new URLSearchParams(search);
    const nonce = params.get('nonce') || new URLSearchParams(hash).get('nonce') || '';
    return String(nonce || '').trim();
  } catch {
    return '';
  }
}

export function unwrapParentResponseData(result, fallback) {
  return unwrapCompatResponseData(result, fallback);
}
