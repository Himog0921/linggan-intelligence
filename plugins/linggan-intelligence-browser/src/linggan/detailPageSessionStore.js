import Dexie from 'dexie';
import {
  packageComments,
  packageContentDetail,
  packageMediaSlots,
  packageReplies,
} from './producerRuntime.js';

const database = new Dexie('LingganDetailPageSessionCache');
database.version(1).stores({
  sessions: '&cacheKey, leaseRef, contentExternalId, expiresAt, updatedAt',
});
database.version(2).stores({
  sessions: '&cacheKey, leaseRef, contentExternalId, expiresAt, updatedAt',
  navigationGrants: '&grantKey, taskId, leaseRef, contentExternalId, state, updatedAt',
});
database.version(3).stores({
  // A page payload belongs to the original lease/task identities, but its
  // retention must not end when that lease expires. It is removable only
  // after every frozen lane has entered the durable outbox.
  sessions: '&cacheKey, leaseRef, contentExternalId, prunableAt, updatedAt',
  navigationGrants: '&grantKey, taskId, leaseRef, contentExternalId, state, updatedAt',
});

const PLAN_VERSION = 'linggan.detail-page-session.v1';
const ALLOWED_LANES = new Set(['content_detail', 'media_slots', 'comments', 'replies']);
const MAX_CACHE_TTL_SECONDS = 6 * 60 * 60;
const MAX_SESSION_JSON_CHARS = 8 * 1024 * 1024;
const NOT_PRUNABLE_AT = Number.MAX_SAFE_INTEGER;

function text(value = '') {
  return String(value || '').trim();
}

function plain(value) {
  return value === undefined ? undefined : JSON.parse(JSON.stringify(value));
}

function contentIdFromTask(taskSpec = {}) {
  return text(taskSpec?.target?.contentExternalId);
}

function capabilityFromTask(taskSpec = {}) {
  return Array.isArray(taskSpec?.capabilitiesRequested)
    ? text(taskSpec.capabilitiesRequested[0])
    : '';
}

export function detailPageSessionCacheKey(leaseRef, contentExternalId) {
  const lease = text(leaseRef);
  const content = text(contentExternalId);
  if (!lease || !content) throw new Error('detail_page_session_identity_required');
  return `${lease}:${content}`;
}

export function detailPageNavigationGrantKey(leaseRef, taskSpec = {}) {
  const lease = text(leaseRef);
  const taskId = text(taskSpec?.taskId);
  const contentExternalId = contentIdFromTask(taskSpec);
  if (!lease || !taskId || !contentExternalId) throw new Error('detail_page_navigation_grant_identity_required');
  return `${lease}:${taskId}:${contentExternalId}`;
}

export function validateDetailPageSessionPlan(plan = {}, expectedContentExternalId = '') {
  if (plan?.contractVersion !== PLAN_VERSION) throw new Error('detail_page_session_plan_version_invalid');
  const contentExternalId = text(plan.contentExternalId);
  if (!contentExternalId || contentExternalId !== text(expectedContentExternalId)) {
    throw new Error('detail_page_session_target_mismatch');
  }
  const lanes = Array.isArray(plan.lanes) ? [...new Set(plan.lanes.map(text).filter(Boolean))] : [];
  if (!lanes.includes('content_detail') || lanes.some((lane) => !ALLOWED_LANES.has(lane))) {
    throw new Error('detail_page_session_lanes_invalid');
  }
  const commentLimit = Number(plan.commentLimit);
  const replyExpandLimit = Number(plan.replyExpandLimit);
  const cacheTtlSeconds = Number(plan.cacheTtlSeconds);
  if (!Number.isInteger(commentLimit) || commentLimit < 0 || commentLimit > 30) {
    throw new Error('detail_page_session_comment_limit_invalid');
  }
  if (!Number.isInteger(replyExpandLimit) || replyExpandLimit < 0) {
    throw new Error('detail_page_session_reply_limit_invalid');
  }
  if (!Number.isFinite(cacheTtlSeconds) || cacheTtlSeconds <= 0) {
    throw new Error('detail_page_session_ttl_invalid');
  }
  return {
    contractVersion: PLAN_VERSION,
    contentExternalId,
    lanes,
    commentLimit,
    replyExpandLimit,
    cacheTtlSeconds: Math.min(MAX_CACHE_TTL_SECONDS, Math.floor(cacheTtlSeconds)),
  };
}

export function packageDetailPageSessionLane(entry = {}, taskSpec = {}) {
  const capability = capabilityFromTask(taskSpec);
  const contentExternalId = contentIdFromTask(taskSpec);
  if (!ALLOWED_LANES.has(capability) || !contentExternalId) {
    throw new Error('detail_page_session_task_invalid');
  }
  if (text(entry.contentExternalId) !== contentExternalId || !entry?.plan?.lanes?.includes(capability)) {
    throw new Error('detail_page_session_task_out_of_scope');
  }
  const common = { platform: taskSpec.platform, observedAt: entry.observedAt, capturedAt: entry.capturedAt };
  if (capability === 'content_detail') {
    return packageContentDetail({ ...common, note: entry.note });
  }
  if (capability === 'media_slots') {
    return packageMediaSlots({ ...common, note: entry.note });
  }
  if (capability === 'comments') {
    return packageComments({ ...common, result: entry.commentResult, noteId: contentExternalId });
  }
  return packageReplies({ ...common, result: entry.commentResult, noteId: contentExternalId });
}

/**
 * Return the bounded page receipt for the first, page-owning detail lane.
 *
 * The background may only treat a page action as started when the response
 * echoes the exact action/capability/task identity it dispatched.  A detail
 * session is no exception: it persists the page result for later separately
 * claimed lanes, but its own `content_detail` task still has to identify
 * itself before its outbox submission can be delivered.
 */
export function detailPageSessionExecutionReceipt({ action, taskSpec = {}, delivery, submissionId } = {}) {
  const normalizedAction = text(action);
  const taskId = text(taskSpec.taskId);
  const capability = capabilityFromTask(taskSpec);
  if (!normalizedAction || !taskId || capability !== 'content_detail') {
    throw new Error('detail_page_session_receipt_identity_invalid');
  }
  return {
    success: true,
    state: 'detail_page_session_queued',
    delivery: text(delivery) || 'pending',
    submissionId: text(submissionId),
    action: normalizedAction,
    capability,
    taskId,
    message: '当前详情页已完整读取；详情已进入待交付队列，其余已批准通道将复用本次页面结果。',
  };
}

export function createDetailPageSessionStore(table = database.sessions, now = () => Date.now()) {
  function allFrozenLanesQueued(row) {
    const queuedCapabilities = new Set(Object.values(row?.queuedTasks || {}).map(text));
    return Array.isArray(row?.plan?.lanes)
      && row.plan.lanes.every((capability) => queuedCapabilities.has(text(capability)));
  }

  async function pruneReliablyQueued() {
    // The outbox owns delivery after a lane has been queued. Before that
    // point, a page payload may be old but is still the only locally retained
    // copy of facts already read from the platform, so it is never evicted by
    // age or cache pressure.
    const prunable = await table.where('prunableAt').belowOrEqual(now()).primaryKeys();
    if (prunable.length > 0) await table.bulkDelete(prunable);
    return prunable.length;
  }

  return {
    async put({ leaseRef, plan, note, commentResult, receipt } = {}) {
      await pruneReliablyQueued();
      const contentExternalId = text(note?.noteId || note?.platformContentId || note?.contentId);
      const normalizedPlan = validateDetailPageSessionPlan(plan, contentExternalId);
      const cacheKey = detailPageSessionCacheKey(leaseRef, contentExternalId);
      const timestamp = now();
      const existing = await table.get(cacheKey);
      const row = {
        cacheKey,
        leaseRef: text(leaseRef),
        contentExternalId,
        plan: normalizedPlan,
        note: plain(note),
        commentResult: plain(commentResult || { total: 0, comments: [], stopReason: 'not_observed' }),
        receipt: plain(receipt || {}),
        observedAt: text(note?.observedAt || note?.collectedAt) || new Date(timestamp).toISOString(),
        capturedAt: new Date(timestamp).toISOString(),
        queuedTasks: existing?.queuedTasks && typeof existing.queuedTasks === 'object'
          ? existing.queuedTasks
          : {},
        deliveryCompleteAt: null,
        prunableAt: NOT_PRUNABLE_AT,
        createdAt: existing?.createdAt || timestamp,
        updatedAt: timestamp,
      };
      if (JSON.stringify(row).length > MAX_SESSION_JSON_CHARS) {
        throw new Error('detail_page_session_too_large');
      }
      await table.put(row);
      await pruneReliablyQueued();
      return row;
    },

    async getForTask({ leaseRef, taskSpec } = {}) {
      const contentExternalId = contentIdFromTask(taskSpec);
      const capability = capabilityFromTask(taskSpec);
      if (!contentExternalId || !ALLOWED_LANES.has(capability)) return null;
      const cacheKey = detailPageSessionCacheKey(leaseRef, contentExternalId);
      const row = await table.get(cacheKey);
      if (!row) return null;
      validateDetailPageSessionPlan(row.plan, contentExternalId);
      if (!row.plan.lanes.includes(capability)) return null;
      return {
        ...row,
        capability,
        alreadyQueued: text(row.queuedTasks?.[taskSpec.taskId]) === capability,
      };
    },

    async markTaskQueued(cacheKey, capability, taskId) {
      const row = await table.get(cacheKey);
      if (!row) return false;
      const queuedTasks = { ...(row.queuedTasks || {}), [text(taskId)]: text(capability) };
      const timestamp = now();
      const fullyQueued = allFrozenLanesQueued({ ...row, queuedTasks });
      await table.update(cacheKey, {
        queuedTasks,
        deliveryCompleteAt: fullyQueued ? (row.deliveryCompleteAt || timestamp) : null,
        prunableAt: fullyQueued ? timestamp : NOT_PRUNABLE_AT,
        updatedAt: timestamp,
      });
      return true;
    },

    async pruneExpired() {
      return pruneReliablyQueued();
    },
  };
}

/**
 * Durable, browser-local side of the detail navigation boundary.  The
 * transaction is intentionally injected so tests can prove the same
 * read-modify-write boundary without relying on a running browser.  Dexie's
 * transaction promise resolves only after IndexedDB commits; callers must
 * await it before opening a tab.
 */
export function createDetailPageNavigationGrantStore({
  table = database.navigationGrants,
  transaction = (work) => database.transaction('rw', table, work),
  now = () => Date.now(),
  createRequestId = () => crypto.randomUUID(),
} = {}) {
  return {
    async prepare({ leaseRef, taskSpec } = {}) {
      const grantKey = detailPageNavigationGrantKey(leaseRef, taskSpec);
      return transaction(async () => {
        const existing = await table.get(grantKey);
        if (existing) return existing;
        const timestamp = now();
        const row = {
          grantKey,
          taskId: text(taskSpec.taskId),
          leaseRef: text(leaseRef),
          contentExternalId: contentIdFromTask(taskSpec),
          grantRequestId: createRequestId(),
          state: 'prepared',
          createdAt: timestamp,
          updatedAt: timestamp,
        };
        await table.add(row);
        return row;
      });
    },

    async attachServerGrant({ grantKey, sessionRef, plan } = {}) {
      return transaction(async () => {
        const row = await table.get(text(grantKey));
        if (!row) throw new Error('detail_page_navigation_grant_missing');
        if (row.sessionRef && row.sessionRef !== text(sessionRef)) {
          throw new Error('detail_page_navigation_session_identity_conflict');
        }
        await table.update(row.grantKey, {
          sessionRef: text(sessionRef), plan: plain(plan),
          state: row.state === 'prepared' ? 'grant_accepted' : row.state,
          updatedAt: now(),
        });
        return {
          ...row, sessionRef: text(sessionRef), plan: plain(plan),
          state: row.state === 'prepared' ? 'grant_accepted' : row.state, updatedAt: now(),
        };
      });
    },

    async consume({ grantKey, sessionRef } = {}) {
      return transaction(async () => {
        const row = await table.get(text(grantKey));
        if (!row) throw new Error('detail_page_navigation_grant_missing');
        if (!text(sessionRef) || row.sessionRef !== text(sessionRef)) {
          throw new Error('detail_page_navigation_session_identity_conflict');
        }
        if (row.state !== 'grant_accepted') return { shouldNavigate: false, row };
        const timestamp = now();
        const consumed = { ...row, state: 'consumed', consumedAt: timestamp, updatedAt: timestamp };
        await table.put(consumed);
        return { shouldNavigate: true, row: consumed };
      });
    },

    async recordWindow({ grantKey, windowId, tabId } = {}) {
      return transaction(async () => {
        const row = await table.get(text(grantKey));
        if (!row || row.state !== 'consumed') throw new Error('detail_page_navigation_not_consumed');
        const timestamp = now();
        await table.update(row.grantKey, {
          state: 'page_opened', windowId: Number(windowId) || null, tabId: Number(tabId) || null,
          openedAt: timestamp, updatedAt: timestamp,
        });
      });
    },

    async markActionDispatched({ grantKey } = {}) {
      return transaction(async () => {
        const row = await table.get(text(grantKey));
        if (!row || row.state !== 'page_opened') throw new Error('detail_page_navigation_page_not_opened');
        const timestamp = now();
        await table.update(row.grantKey, { state: 'action_dispatched', actionDispatchedAt: timestamp, updatedAt: timestamp });
      });
    },

    async get({ leaseRef, taskSpec } = {}) {
      return table.get(detailPageNavigationGrantKey(leaseRef, taskSpec));
    },
  };
}

export const detailPageSessionStore = createDetailPageSessionStore();
export const detailPageNavigationGrantStore = createDetailPageNavigationGrantStore();
