import { generateCsv, downloadFile, getUnifiedAuthorHandle } from '../shared/utils.js';
import { unwrapCompatResponseData } from '../shared/responseEnvelope.js';
import { normalizeAuthorRecord, normalizeCommentRecord, normalizeNoteRecord } from '../db/recordNormalization.js';

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

function pickQualityMeta(item = {}) {
  return {
    dataQuality: String(item?.dataQuality || '').trim() || 'full',
    qualityReason: String(item?.qualityReason || '').trim(),
    sourceTier: String(item?.sourceTier || '').trim(),
    collectionRunId: String(item?.collectionRunId || '').trim(),
  };
}

export const DASHBOARD_SYNC_TO_WORKBENCH_TIMEOUT_MS = 45000;

function countValue(value, fallback = 0) {
  const parsed = Number(value);
  return Number.isFinite(parsed) ? Math.max(0, Math.floor(parsed)) : fallback;
}

export function summarizeWorkbenchSyncResult(tab = '', selectedCount = 0, result = {}) {
  const normalizedTab = String(tab || '').trim();
  const meta = result?.meta && typeof result.meta === 'object' ? result.meta : {};
  const total = countValue(selectedCount);
  let imported = countValue(result?.imported);
  let skipped = countValue(result?.skipped);
  let invalid = 0;
  let outcomeText = '';
  let existing = 0;
  let monitorOutcomeConfirmed = false;
  // 计数只是服务端返回的描述，不能代替“账本已写入并关联”的明确确认。
  // 老版本接口缺少此字段时宁可显示未确认，也不能把同步成功误报为媒体已登记。
  const mediaRegistrationConfirmed = meta.mediaRegistrationConfirmed === true;
  const mediaObserved = countValue(meta.mediaObserved);
  const mediaEnqueued = countValue(meta.mediaEnqueued);
  const mediaIncompleteCount = [
    meta.mediaInvalid,
    meta.mediaConflicted,
    meta.mediaRejected,
    meta.mediaRequiredMissing,
    meta.mediaUnlinked,
  ].reduce((totalCount, value) => totalCount + countValue(value), 0);
  const mediaIncomplete = mediaIncompleteCount > 0;

  if (normalizedTab === 'comments') {
    const received = countValue(meta.commentsReceived, total);
    imported = countValue(meta.commentsProcessed, countValue(meta.commentsRegistered, imported));
    skipped = countValue(meta.commentsSkipped, skipped);
    invalid = countValue(meta.commentsInvalid);
    const queued = countValue(meta.commentsQueued);
    const failed = countValue(meta.commentsFailed);
    const outcome = [`已接收 ${received} 条`, `已入库 ${imported} 条`, `已跳过 ${skipped} 条`];
    if (invalid > 0) outcome.push(`无效 ${invalid} 条`);
    if (queued > 0) outcome.push(`待处理 ${queued} 条`);
    if (failed > 0) outcome.push(`待重试 ${failed} 条`);
    outcomeText = outcome.join('，');
  } else if (normalizedTab === 'authors') {
    monitorOutcomeConfirmed = [
      'monitorSourcesCreated',
      'monitorSourcesExisting',
      'monitorSourcesSkipped',
    ].some((key) => Object.prototype.hasOwnProperty.call(meta, key));
    if (monitorOutcomeConfirmed) {
      imported = countValue(meta.monitorSourcesCreated);
      existing = countValue(meta.monitorSourcesExisting);
      skipped = countValue(meta.monitorSourcesSkipped);
      const outcome = [];
      if (imported > 0) outcome.push(`已新增监控来源 ${imported} 条`);
      if (existing > 0) outcome.push(`监控来源已存在 ${existing} 条`);
      if (skipped > 0) outcome.push(`未创建 ${skipped} 条`);
      outcomeText = outcome.join('，') || '没有可创建的监控来源';
    } else {
      imported = 0;
      skipped = 0;
      outcomeText = '工作台未确认监控来源写入，请刷新监控中心核对';
    }
  } else if (normalizedTab === 'notes' && mediaRegistrationConfirmed) {
    const outcome = [];
    if (mediaObserved > 0) outcome.push(`媒体已登记 ${mediaObserved} 项`);
    if (mediaEnqueued > 0) outcome.push(`已进入处理队列 ${mediaEnqueued} 项`);
    if (mediaIncomplete) outcome.push(`媒体登记异常 ${mediaIncompleteCount} 项`);
    outcomeText = outcome.join('，');
  } else if (normalizedTab === 'notes') {
    outcomeText = '工作台未确认媒体账本登记';
  }

  const details = [];
  if (meta.notesReceived != null) details.push(`笔记 ${countValue(meta.notesReceived)}`);
  if (meta.commentsReceived != null) details.push(`评论 ${countValue(meta.commentsReceived)}`);
  if (meta.authorsReceived != null) details.push(`博主 ${countValue(meta.authorsReceived)}`);

  return {
    imported,
    skipped,
    invalid,
    existing,
    monitorOutcomeConfirmed,
    mediaRegistrationConfirmed,
    mediaIncomplete,
    total,
    details,
    detailText: details.length ? `（${details.join('，')}）` : '',
    outcomeText,
    failReason: String(meta.failReason || meta.errorReason || '').trim(),
  };
}

export function buildWorkbenchSyncPayload(tab = '', items = [], { extractedAt = Date.now() } = {}) {
  const selectedItems = Array.isArray(items) ? items : [];
  const normalizedTab = String(tab || '').trim();

  if (normalizedTab === 'notes') {
    return {
      notes: selectedItems.map((note) => {
        const normalized = normalizeNoteRecord(note);
        return {
          ...normalized,
          noteId: normalized.noteId,
          title: normalized.title,
          content: normalized.content,
          hashtags: normalized.hashtags,
          images: normalized.images,
          platform: normalized.platform || 'xhs',
          authorName: normalized.authorName,
          likes: normalized.likes || 0,
          collects: normalized.collects || 0,
          comments: normalized.comments || 0,
          url: getPreferredRecordUrl(normalized, 'noteUrl') || normalized.url || normalized.noteUrl,
          createdAt: normalized.createdAt,
          extractedAt,
          source: '灵感爆爆爆插件',
          ...pickQualityMeta(normalized),
        };
      }),
    };
  }

  if (normalizedTab === 'comments') {
    return {
      comments: selectedItems.map((comment) => {
        const normalized = normalizeCommentRecord(comment);
        return {
          ...normalized,
          commentId: normalized.commentId,
          contentId: normalized.contentId,
          text: normalized.text,
          author: normalized.author,
          authorId: normalized.authorId,
          replyToUserName: normalized.replyToUserName,
          likes: normalized.likes || 0,
          platform: normalized.platform || 'xhs',
          level: normalized.level || 1,
          rootCommentId: normalized.rootCommentId,
          parentCommentId: normalized.parentCommentId,
          url: getPreferredRecordUrl(normalized, 'noteUrl'),
          createdAt: normalized.createdAt,
          extractedAt,
          source: '灵感爆爆爆插件',
          ...pickQualityMeta(normalized),
        };
      }),
    };
  }

  if (normalizedTab === 'authors') {
    return {
      authors: selectedItems.map((author) => {
        const normalized = normalizeAuthorRecord(author);
        return {
          ...normalized,
          userId: normalized.userId,
          nickname: normalized.name || normalized.nickname || '',
          avatar: normalized.avatar || normalized.avatarUrl || '',
          signature: normalized.description || normalized.signature || normalized.bio || '',
          fansCount: normalized.fans || normalized.fansCount || 0,
          followingCount: normalized.follows || normalized.followingCount || 0,
          notesCount: normalized.interactions || normalized.notesCount || 0,
          likedCount: normalized.interactions || normalized.likedCount || 0,
          collectedCount: normalized.collectedCount || 0,
          platform: normalized.platform || 'xhs',
          ipLocation: normalized.ipLocation || normalized.location || '',
          url: normalized.profileUrl || normalized.url || '',
          createdAt: normalized.createdAt,
          extractedAt,
          source: '灵感爆爆爆插件',
          ...pickQualityMeta(normalized),
        };
      }),
    };
  }

  return {};
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
