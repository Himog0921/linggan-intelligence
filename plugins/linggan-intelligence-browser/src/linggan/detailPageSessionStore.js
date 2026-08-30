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

const PLAN_VERSION = 'linggan.detail-page-session.v1';
const ALLOWED_LANES = new Set(['content_detail', 'media_slots', 'comments', 'replies']);
const MAX_CACHE_TTL_SECONDS = 6 * 60 * 60;

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

export function createDetailPageSessionStore(table = database.sessions, now = () => Date.now()) {
  return {
    async put({ leaseRef, plan, note, commentResult, receipt } = {}) {
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
        expiresAt: timestamp + (normalizedPlan.cacheTtlSeconds * 1000),
        createdAt: existing?.createdAt || timestamp,
        updatedAt: timestamp,
      };
      await table.put(row);
      return row;
    },

    async getForTask({ leaseRef, taskSpec } = {}) {
      const contentExternalId = contentIdFromTask(taskSpec);
      const capability = capabilityFromTask(taskSpec);
      if (!contentExternalId || !ALLOWED_LANES.has(capability)) return null;
      const cacheKey = detailPageSessionCacheKey(leaseRef, contentExternalId);
      const row = await table.get(cacheKey);
      if (!row) return null;
      if (Number(row.expiresAt || 0) <= now()) {
        await table.delete(cacheKey);
        return null;
      }
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
      await table.update(cacheKey, {
        queuedTasks: { ...(row.queuedTasks || {}), [text(taskId)]: text(capability) },
        updatedAt: now(),
      });
      return true;
    },

    async pruneExpired() {
      const expired = await table.where('expiresAt').belowOrEqual(now()).primaryKeys();
      if (expired.length > 0) await table.bulkDelete(expired);
      return expired.length;
    },
  };
}

export const detailPageSessionStore = createDetailPageSessionStore();
