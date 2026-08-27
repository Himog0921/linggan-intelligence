/**
 * 内容工作台同步模块
 * 负责将插件采集的数据同步到选题工作台
 */

import { normalizeServerUrl } from '../shared/utils.js';
import { enrichNoteWithDataFoundationPayload } from '../workbench/runtime/dataFoundationPayload.js';

const API_BASE_URL = 'https://lingganboom.fun';
const FLYWHEEL_STORAGE_KEY = 'flywheelConfig';

let cachedConfig = null;
let cachedAt = 0;
const CACHE_TTL_MS = 30000;
const DATA_SESSION_REFRESH_SKEW_MS = 5 * 60 * 1000;

async function readFlywheelStorage() {
  if (cachedConfig && (Date.now() - cachedAt) < CACHE_TTL_MS) return cachedConfig;
  if (!globalThis.chrome?.storage?.local) {
    return { serverUrl: normalizeServerUrl(API_BASE_URL), enabled: true, apiToken: '' };
  }
  const result = await chrome.storage.local.get(FLYWHEEL_STORAGE_KEY);
  const config = result?.[FLYWHEEL_STORAGE_KEY] || {};
  const hasEnabledFlag = Object.prototype.hasOwnProperty.call(config, 'enabled');
  cachedConfig = {
    serverUrl: normalizeServerUrl(config.serverUrl || API_BASE_URL),
    enabled: hasEnabledFlag ? config.enabled !== false : true,
    apiToken: String(config.apiToken || '').trim(),
    dataToken: String(config.dataToken || '').trim(),
    dataTokenExpiresAt: String(config.dataTokenExpiresAt || '').trim(),
    dataWorkspaceId: String(config.dataWorkspaceId || '').trim(),
    dataUserEmail: String(config.dataUserEmail || '').trim(),
    dataUserName: String(config.dataUserName || '').trim(),
    updatedAt: config.updatedAt || 0,
  };
  cachedAt = Date.now();
  return cachedConfig;
}

async function writeFlywheelStorage(config = {}) {
  const previous = await readFlywheelStorage();
  const hasServerUrl = Object.prototype.hasOwnProperty.call(config, 'serverUrl');
  const hasEnabledFlag = Object.prototype.hasOwnProperty.call(config, 'enabled');
  const hasApiToken = Object.prototype.hasOwnProperty.call(config, 'apiToken');
  const hasDataToken = Object.prototype.hasOwnProperty.call(config, 'dataToken');
  const hasDataTokenExpiresAt = Object.prototype.hasOwnProperty.call(config, 'dataTokenExpiresAt');
  const hasDataWorkspaceId = Object.prototype.hasOwnProperty.call(config, 'dataWorkspaceId');
  const hasDataUserEmail = Object.prototype.hasOwnProperty.call(config, 'dataUserEmail');
  const hasDataUserName = Object.prototype.hasOwnProperty.call(config, 'dataUserName');
  const next = {
    serverUrl: normalizeServerUrl(
      hasServerUrl
        ? (config.serverUrl || API_BASE_URL)
        : (previous.serverUrl || API_BASE_URL),
    ),
    enabled: hasEnabledFlag ? config.enabled !== false : previous.enabled !== false,
    apiToken: hasApiToken ? String(config.apiToken || '').trim() : String(previous.apiToken || '').trim(),
    dataToken: hasDataToken ? String(config.dataToken || '').trim() : String(previous.dataToken || '').trim(),
    dataTokenExpiresAt: hasDataTokenExpiresAt ? String(config.dataTokenExpiresAt || '').trim() : String(previous.dataTokenExpiresAt || '').trim(),
    dataWorkspaceId: hasDataWorkspaceId ? String(config.dataWorkspaceId || '').trim() : String(previous.dataWorkspaceId || '').trim(),
    dataUserEmail: hasDataUserEmail ? String(config.dataUserEmail || '').trim() : String(previous.dataUserEmail || '').trim(),
    dataUserName: hasDataUserName ? String(config.dataUserName || '').trim() : String(previous.dataUserName || '').trim(),
    updatedAt: Date.now(),
  };
  cachedConfig = next;
  cachedAt = Date.now();
  if (globalThis.chrome?.storage?.local) {
    await chrome.storage.local.set({
      [FLYWHEEL_STORAGE_KEY]: next,
    });
  }
  return next;
}

// 监听 storage 变化，外部修改配置时立即清空缓存
if (globalThis.chrome?.storage?.onChanged) {
  chrome.storage.onChanged.addListener((changes, area) => {
    if (area === 'local' && changes[FLYWHEEL_STORAGE_KEY]) {
      cachedConfig = null;
      cachedAt = 0;
    }
  });
}

async function resolveFlywheelBaseUrl(serverUrl = '') {
  if (serverUrl) return normalizeServerUrl(serverUrl);
  const config = await readFlywheelStorage();
  return normalizeServerUrl(config.serverUrl || API_BASE_URL);
}

async function fetchFlywheel(path, options = {}) {
  const {
    serverUrl = '',
    apiToken,
    dataToken,
    timeoutMs = 10000,
    method = 'GET',
    headers = {},
    body,
  } = options;
  const baseUrl = await resolveFlywheelBaseUrl(serverUrl);
  const config = await readFlywheelStorage();
  const resolvedToken = typeof apiToken === 'string'
    ? String(apiToken || '').trim()
    : String(config?.apiToken || '').trim();
  const resolvedDataToken = typeof dataToken === 'string'
    ? String(dataToken || '').trim()
    : String(config?.dataToken || '').trim();
  const requestHeaders = {
    ...headers,
  };
  if (resolvedToken) {
    requestHeaders.Authorization = `Bearer ${resolvedToken}`;
  }
  if (resolvedDataToken) {
    requestHeaders['X-Plugin-Data-Token'] = resolvedDataToken;
  }
  const response = await fetch(`${baseUrl}${path}`, {
    method,
    headers: requestHeaders,
    credentials: 'include',
    body,
    signal: AbortSignal.timeout(timeoutMs),
  });
  return response;
}

function hasFreshFlywheelDataSession(config = {}) {
  const dataToken = firstText(config.dataToken);
  const expiresAt = Date.parse(firstText(config.dataTokenExpiresAt));
  return Boolean(dataToken)
    && Number.isFinite(expiresAt)
    && expiresAt - Date.now() > DATA_SESSION_REFRESH_SKEW_MS;
}

export async function ensureFlywheelDataSession(config = {}) {
  const stored = await readFlywheelStorage();
  const baseConfig = {
    ...stored,
    ...config,
  };
  const authorizationToken = firstText(baseConfig.apiToken);
  if (!authorizationToken) return baseConfig;

  const baseUrl = await resolveFlywheelBaseUrl(baseConfig.serverUrl || '');
  const response = await fetch(`${baseUrl}/api/plugin-data-workspace`, {
    method: 'POST',
    credentials: 'include',
    headers: {
      Authorization: `Bearer ${authorizationToken}`,
    },
    signal: AbortSignal.timeout(15000),
  }).catch((error) => {
    if (hasFreshFlywheelDataSession(baseConfig)) return null;
    throw error;
  });
  if (!response) return baseConfig;
  if (!response.ok) {
    if (hasFreshFlywheelDataSession(baseConfig)) return baseConfig;
    await throwForWorkbenchHttpError(response, 'plugin_data_workspace_required');
  }

  const data = await response.json().catch(() => ({}));
  const dataToken = firstText(data.dataToken);
  if (!dataToken) {
    throw createHttpError('plugin_data_workspace_required: missing_data_token', { retryable: true });
  }
  const next = {
    ...baseConfig,
    dataToken,
    dataTokenExpiresAt: firstText(data.expiresAt),
    dataWorkspaceId: firstText(data.workspaceId),
    dataUserEmail: firstText(data.user?.email),
    dataUserName: firstText(data.user?.name),
  };
  await writeFlywheelStorage(next);
  return next;
}

export function mergeFlywheelAuthorization(config = {}, authorization = {}) {
  const authorizationToken = firstText(
    authorization?.authorizationToken
    || authorization?.apiToken
    || authorization?.token,
  );
  const authorizationStatus = firstText(
    authorization?.status
    || authorization?.authorizationStatus,
  ).toLowerCase();
  const authorizationUsable = Boolean(authorizationToken)
    && !['revoked', 'expired', 'suspended', 'disabled'].includes(authorizationStatus);

  return {
    ...config,
    apiToken: authorizationUsable
      ? authorizationToken
      : firstText(config?.apiToken),
  };
}

function firstText(value) {
  return typeof value === 'string' && value.trim() ? value.trim() : '';
}

function createHttpError(message, { status = 0, bodyText = '', retryable = false } = {}) {
  const error = new Error(message);
  error.status = status;
  error.bodyText = bodyText;
  error.retryable = Boolean(retryable);
  return error;
}

function isRetryableStatus(status) {
  return [408, 429, 500, 502, 503, 504].includes(Number(status));
}

async function readErrorBody(response) {
  return response.text().catch(() => '');
}

async function throwForWorkbenchHttpError(response, fallbackMessage = 'workbench_request_failed') {
  const bodyText = await readErrorBody(response);
  let retryable = isRetryableStatus(response.status);
  let message = bodyText || `${fallbackMessage}: HTTP ${response.status}`;
  try {
    const body = JSON.parse(bodyText || '{}');
    if (typeof body.retryable === 'boolean') retryable = body.retryable;
    if (firstText(body.error)) message = firstText(body.error);
    if (firstText(body.message)) message = firstText(body.message);
  } catch {
    // keep status-based classification
  }
  throw createHttpError(message, {
    status: response.status,
    bodyText,
    retryable,
  });
}

function normalizeSource(s) {
  const rawData = s.rawData || s;
  const qualityReason = s?.qualityReason ?? rawData?.qualityReason;
  const platformContentId = s.noteId || s.platformContentId || s.platformId || s.id;
  const embeddedComments = [
    s.commentsData,
    s.comments,
    rawData?.commentsData,
    rawData?.comments,
  ].find(Array.isArray) || [];
  const commentCount = s.metrics?.comments
    ?? s.comments_count
    ?? s.commentCount
    ?? (typeof s.comments === 'number' || typeof s.comments === 'string' ? s.comments : undefined);
  const normalized = {
    ...rawData,
    ...s,
    platform: s.platform || 'xhs',
    noteId: s.noteId || platformContentId,
    platformContentId,
    platformId: platformContentId,
    url: s.url,
    title: s.title,
    content: s.content || s.bodyText || s.desc,
    bodyText: s.content || s.bodyText || s.desc,
    coverUrl: s.coverImage || s.coverUrl,
    likes: s.metrics?.likes || s.likes,
    collects: s.metrics?.collects || s.collects,
    comments: commentCount,
    shares: s.metrics?.shares || s.shares,
    authorId: s.author?.id || s.authorId,
    authorName: s.author?.name || s.authorName,
    dataQuality: String(s?.dataQuality ?? rawData?.dataQuality ?? '').trim(),
    qualityReason: qualityReason == null ? '' : String(qualityReason),
    sourceTier: String(s?.sourceTier ?? rawData?.sourceTier ?? '').trim(),
    collectionRunId: String(s?.collectionRunId ?? rawData?.collectionRunId ?? '').trim(),
    rawData,
    commentsData: embeddedComments,
  };
  return enrichNoteWithDataFoundationPayload(normalized, {
    source: 'plugin_manual_sync',
    taskId: normalized.collectionRunId || 'manual_sync',
    externalRecordId: normalized.platformId || normalized.url,
  });
}

const MANUAL_IMPORT_BATCH_SIZE = 50;

function createManualImportBatches(records = {}, batchSize = MANUAL_IMPORT_BATCH_SIZE) {
  const entries = [
    ...(Array.isArray(records.notes) ? records.notes.map((record) => ['notes', record]) : []),
    ...(Array.isArray(records.comments) ? records.comments.map((record) => ['comments', record]) : []),
    ...(Array.isArray(records.authors) ? records.authors.map((record) => ['authors', record]) : []),
  ];
  const batches = [];
  for (let index = 0; index < entries.length; index += batchSize) {
    const batch = { notes: [], comments: [], authors: [] };
    for (const [kind, record] of entries.slice(index, index + batchSize)) {
      batch[kind].push(record);
    }
    batches.push(batch);
  }
  return batches;
}

function embeddedCommentsFromNotes(notes = []) {
  return notes.flatMap((note) => {
    if (!Array.isArray(note?.commentsData)) return [];
    return note.commentsData.map((comment) => ({
      ...comment,
      platform: comment?.platform || note.platform || 'xhs',
      noteId: comment?.noteId || comment?.contentId || note.noteId || note.platformContentId,
    }));
  });
}

function emptyManualImportMeta() {
  return {
    notesReceived: 0,
    notesImported: 0,
    notesSkipped: 0,
    commentsReceived: 0,
    commentsRegistered: 0,
    commentsProcessed: 0,
    commentsQueued: 0,
    commentsFailed: 0,
    commentsSkipped: 0,
    commentsInvalid: 0,
    authorsReceived: 0,
    authorsIngested: 0,
    authorsSkipped: 0,
    monitorSourcesCreated: 0,
    monitorSourcesExisting: 0,
    monitorSourcesSkipped: 0,
    mediaObserved: 0,
    mediaEnqueued: 0,
    mediaSkipped: 0,
    mediaInvalid: 0,
    mediaConflicted: 0,
    mediaRejected: 0,
    mediaRequiredMissing: 0,
    mediaUnlinked: 0,
    mediaRegistrationConfirmed: true,
    jobs: [],
  };
}

function mergeManualImportMeta(total, incoming = {}) {
  const next = { ...total };
  for (const key of Object.keys(total)) {
    if (key === 'jobs') continue;
    if (key === 'mediaRegistrationConfirmed') {
      next[key] = Boolean(total[key]) && Boolean(incoming[key]);
      continue;
    }
    next[key] = Number(total[key] || 0) + Number(incoming[key] || 0);
  }
  next.jobs = [...(total.jobs || []), ...(Array.isArray(incoming.jobs) ? incoming.jobs : [])];
  return next;
}

async function sendManualImportBatch(dataConfig, records, metadata = {}) {
  const response = await fetchFlywheel('/api/execution-tasks/manual-import', {
    ...dataConfig,
    method: 'POST',
    headers: {
      'Content-Type': 'application/json',
    },
    body: JSON.stringify({
      source: 'plugin_manual_sync',
      result: { records },
      metadata,
    }),
    timeoutMs: 45000,
  });

  if (!response.ok) {
    await throwForWorkbenchHttpError(response, 'plugin_manual_import_failed');
  }

  return response.json();
}

export async function syncManualRecordsToWorkbench(config = {}, records = {}, metadata = {}) {
  const dataConfig = await ensureFlywheelDataSession(config);
  const normalizedNotes = (Array.isArray(records.notes) ? records.notes : []).map(normalizeSource);
  const normalizedRecords = {
    notes: normalizedNotes,
    comments: [
      ...(Array.isArray(records.comments) ? records.comments : []),
      ...embeddedCommentsFromNotes(normalizedNotes),
    ],
    authors: Array.isArray(records.authors) ? records.authors : [],
  };
  const batches = createManualImportBatches(normalizedRecords);
  if (batches.length === 0) {
    return { success: true, imported: 0, skipped: 0, details: [], meta: emptyManualImportMeta() };
  }

  let imported = 0;
  let skipped = 0;
  let meta = emptyManualImportMeta();
  const details = [];
  const errors = [];
  let successfulBatchCount = 0;

  for (const batch of batches) {
    try {
      const result = await sendManualImportBatch(dataConfig, batch, metadata);
      successfulBatchCount += 1;
      imported += Number(result.imported) || 0;
      skipped += Number(result.skipped) || 0;
      meta = mergeManualImportMeta(meta, result.meta);
      if (Array.isArray(result.sources)) details.push(...result.sources);
    } catch (error) {
      errors.push(String(error?.message || error || 'plugin_manual_import_failed'));
    }
  }

  if (errors.length > 0 && successfulBatchCount === 0) {
    return { success: false, imported, skipped, details, meta, error: errors.join('; ') };
  }
  return {
    success: true,
    imported,
    skipped,
    details,
    meta,
    ...(errors.length > 0 ? { partial: true, error: errors.join('; ') } : {}),
  };
}

/**
 * 同步采集的数据到内容工作台
 * @param {Array} sources - 采集的笔记数据
 * @param {string} tag - 标签：direct_import | evaluate | analysis_only
 * @param {string} operator - 操作人
 * @returns {Promise<{success: boolean, imported?: number, skipped?: number, error?: string}>}
 */
export async function syncToFlywheel(sources, tag = 'evaluate', operator = 'anonymous') {
  const config = await readFlywheelStorage();
  return syncManualRecordsToWorkbench(
    config,
    { notes: Array.isArray(sources) ? sources : [], comments: [], authors: [] },
    { tag, operator },
  );
}

export async function testConnection(serverUrl = '') {
  try {
    const response = await fetchFlywheel('/api/collect/status', {
      serverUrl,
      timeoutMs: 5000,
    });
    return {
      success: response.ok,
      status: response.status,
    };
  } catch (error) {
    return {
      success: false,
      error: String(error?.message || error || '连接失败'),
    };
  }
}

export async function getFlywheelConfig() {
  return readFlywheelStorage();
}

export async function saveFlywheelConfig(config = {}) {
  return writeFlywheelStorage(config);
}
