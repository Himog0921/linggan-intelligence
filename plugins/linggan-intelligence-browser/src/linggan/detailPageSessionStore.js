import Dexie from 'dexie';
import { validateTaskSpec } from './adapter.js';
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
database.version(4).stores({
  sessions: '&cacheKey, leaseRef, contentExternalId, prunableAt, updatedAt',
  navigationGrants: '&grantKey, taskId, leaseRef, contentExternalId, state, updatedAt',
  // 导航前由服务端登记的通道交付身份。主键就是该通道任务自己的 taskId：交付时要用的正是
  // 「这条任务的服务端身份」，用别的键都会在恢复路径上多绕一层推断。
  lanePreparations: '&taskId, leaseRef, contentExternalId, capability, sessionRef, updatedAt',
});
database.version(5).stores({
  sessions: '&cacheKey, leaseRef, contentExternalId, prunableAt, updatedAt, [prunableAt+updatedAt]',
  navigationGrants: '&grantKey, taskId, leaseRef, contentExternalId, state, updatedAt',
  lanePreparations: '&taskId, leaseRef, contentExternalId, capability, sessionRef, updatedAt',
});

const PLAN_VERSION = 'linggan.detail-page-session.v1';
const ALLOWED_LANES = new Set(['content_detail', 'media_slots', 'comments', 'replies']);
const MAX_CACHE_TTL_SECONDS = 6 * 60 * 60;
const MAX_SESSION_JSON_CHARS = 8 * 1024 * 1024;
const NOT_PRUNABLE_AT = Number.MAX_SAFE_INTEGER;

function text(value = '') {
  return String(value || '').trim();
}

// The page collector exposes observedAt as an epoch millisecond number. The
// Producer contract, however, accepts an RFC3339 timestamp. Normalize once at
// the durable cache boundary so every later lane reuses the same valid fact.
export function canonicalCaptureTimestamp(value, fallbackTimestamp = null) {
  if (typeof value === 'string' && value.trim()) {
    const parsed = Date.parse(value);
    if (Number.isFinite(parsed)) return new Date(parsed).toISOString();
  }
  const numeric = typeof value === 'number' ? value : Number(String(value || '').trim());
  if (Number.isFinite(numeric) && numeric > 0) {
    const milliseconds = numeric < 10_000_000_000 ? numeric * 1000 : numeric;
    const parsed = new Date(milliseconds);
    if (!Number.isNaN(parsed.getTime())) return parsed.toISOString();
  }
  return fallbackTimestamp == null ? '' : new Date(fallbackTimestamp).toISOString();
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
    async put({ leaseRef, plan, note, commentResult, receipt, detailTaskId, detailPackage } = {}) {
      await pruneReliablyQueued();
      const contentExternalId = text(note?.noteId || note?.platformContentId || note?.contentId);
      const normalizedPlan = validateDetailPageSessionPlan(plan, contentExternalId);
      const cacheKey = detailPageSessionCacheKey(leaseRef, contentExternalId);
      const timestamp = now();
      const persist = async () => {
        const existing = await table.get(cacheKey);
        if (existing) {
          if (JSON.stringify([existing.note, existing.plan, existing.commentResult])
              !== JSON.stringify([plain(note), normalizedPlan,
                plain(commentResult || { total: 0, comments: [], stopReason: 'not_observed' })])) {
            throw new Error('detail_page_session_content_conflict');
          }
          return existing;
        }
        const row = {
          cacheKey,
          leaseRef: text(leaseRef),
          contentExternalId,
          plan: normalizedPlan,
          note: plain(note),
          commentResult: plain(commentResult || { total: 0, comments: [], stopReason: 'not_observed' }),
          receipt: plain(receipt || {}),
          packages: detailTaskId && detailPackage ? { [detailTaskId]: plain(detailPackage) } : {},
          observedAt: canonicalCaptureTimestamp(note?.observedAt)
            || canonicalCaptureTimestamp(note?.collectedAt)
            || canonicalCaptureTimestamp(timestamp, timestamp),
          capturedAt: canonicalCaptureTimestamp(timestamp, timestamp),
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
      await (table.add ? table.add(row) : table.put(row));
      return row;
      };
      // Production Dexie serializes the first write with its conflict check.
      // Injected test tables provide the same transaction boundary below.
      return table.db ? table.db.transaction('rw', table, persist) : persist();
    },

    async pending({ limit = 20 } = {}) {
      return table.where('[prunableAt+updatedAt]')
        .between([NOT_PRUNABLE_AT, Dexie.minKey], [NOT_PRUNABLE_AT, Dexie.maxKey])
        .limit(limit).toArray();
    },

    async freezePackage({ cacheKey, taskSpec } = {}) {
      return table.db.transaction('rw', table, async () => {
        const row = await table.get(cacheKey);
        if (!row) throw new Error('detail_page_session_missing');
        const existing = row.packages?.[taskSpec.taskId];
        if (existing) return existing;
        const capturePackage = packageDetailPageSessionLane(row, taskSpec);
        await table.update(cacheKey, { packages: { ...row.packages, [taskSpec.taskId]: capturePackage } });
        return capturePackage;
      });
    },

    async recordRecoveryError(cacheKey, reason) {
      await table.update(cacheKey, { recoveryError: String(reason), updatedAt: now() });
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
      const mark = async () => {
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
      };
      return table.db ? table.db.transaction('rw', table, mark) : mark();
    },

    async pruneExpired() {
      return pruneReliablyQueued();
    },
  };
}

/**
 * 服务端在导航前登记的通道交付身份，本机持久副本。
 *
 * 它记录的是「这条通道任务已经有一个服务端铸造的身份」，不是「这条通道已经采到东西」。
 * 交付时用它作为 startAttempt 的身份：断网期间取回的包因此仍能拿到原来那个身份，
 * 而不是在租约关闭后凭空生成一个服务端不认识的新身份。
 */
export function createDetailPageLanePreparationStore({
  table = database.lanePreparations,
  transaction = (work) => database.transaction('rw', table, work),
  now = () => Date.now(),
} = {}) {
  function rowFrom(lane, context) {
    return {
      taskId: text(lane?.taskId),
      attemptId: text(lane?.attemptId),
      capability: text(lane?.capability),
      taskSpec: plain(lane?.taskSpec),
      leaseExpiresAt: text(lane?.leaseExpiresAt),
      sessionRef: text(context.sessionRef),
      leaseRef: text(context.leaseRef),
      contentExternalId: text(context.contentExternalId),
      updatedAt: now(),
    };
  }

  return {
    /**
     * 写入一次准备回执。同一通道任务拿到同一个身份时是幂等的重放；拿到不同身份、
     * 不同会话或不同能力时是身份冲突——那说明这不是同一次准备，不能覆盖旧记录。
     */
    async recordLanes({ sessionRef, leaseRef, contentExternalId, lanes } = {}) {
      const rows = (Array.isArray(lanes) ? lanes : []).map((lane) => rowFrom(lane, {
        sessionRef, leaseRef, contentExternalId,
      }));
      if (rows.length === 0) return [];
      for (const row of rows) {
        if (!row.taskId || !row.attemptId || !ALLOWED_LANES.has(row.capability)
            || !Number.isFinite(Date.parse(row.leaseExpiresAt))) {
          throw new Error('detail_page_lane_preparation_invalid');
        }
        validateTaskSpec(row.taskSpec);
        if (row.taskSpec.taskId !== row.taskId || row.taskSpec.source !== 'scheduled'
            || row.taskSpec.target.contentExternalId !== row.contentExternalId
            || row.taskSpec.capabilitiesRequested[0] !== row.capability) {
          throw new Error('detail_page_lane_preparation_invalid');
        }
      }
      return transaction(async () => {
        for (const row of rows) {
          const existing = await table.get(row.taskId);
          if (!existing) continue;
          if (text(existing.attemptId) !== row.attemptId
              || text(existing.sessionRef) !== row.sessionRef
              || text(existing.capability) !== row.capability
              || text(existing.leaseRef) !== row.leaseRef
              || text(existing.contentExternalId) !== row.contentExternalId
              || (existing.taskSpec && JSON.stringify(existing.taskSpec) !== JSON.stringify(row.taskSpec))) {
            throw new Error('detail_page_lane_preparation_conflict');
          }
        }
        for (const row of rows) await table.put(row);
        return rows;
      });
    },

    async get(taskId) {
      const key = text(taskId);
      return key ? (await table.get(key)) || null : null;
    },
    async forSession(leaseRef, contentExternalId) {
      const rows = await table.where('leaseRef').equals(text(leaseRef)).toArray();
      return rows.filter((row) => row.contentExternalId === text(contentExternalId));
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
  // 交付身份只在该店的写事务里登记（见 attachServerGrant），所以这一层的「事务」
  // 就是父事务本身，不再另开一层。
  lanePreparationStore = createDetailPageLanePreparationStore({
    transaction: (work) => work(),
  }),
  transaction = (work) => database.transaction(
    'rw', table, database.lanePreparations, work,
  ),
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

    async attachServerGrant({ grantKey, sessionRef, plan, lanePreparation = null } = {}) {
      return transaction(async () => {
        const row = await table.get(text(grantKey));
        if (!row) throw new Error('detail_page_navigation_grant_missing');
        if (row.sessionRef && row.sessionRef !== text(sessionRef)) {
          throw new Error('detail_page_navigation_session_identity_conflict');
        }
        // 交付身份与导航授权同库同事务落盘：只写下一半，重启后就会既打不开页面、
        // 也说不清哪条通道已经拿到过身份。
        if (Array.isArray(lanePreparation?.lanes) && lanePreparation.lanes.length > 0) {
          await lanePreparationStore.recordLanes({
            sessionRef,
            leaseRef: row.leaseRef,
            contentExternalId: row.contentExternalId,
            lanes: lanePreparation.lanes,
          });
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
export const detailPageLanePreparationStore = createDetailPageLanePreparationStore();
export const detailPageNavigationGrantStore = createDetailPageNavigationGrantStore();
