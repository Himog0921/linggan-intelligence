import {
  WORKBENCH_EVENT_SOURCE,
  WORKBENCH_PROTOCOL_VERSION,
} from './schema.js';

function normalizeText(value = '') {
  return String(value || '').trim();
}

function normalizeObject(value) {
  return value && typeof value === 'object' && !Array.isArray(value) ? value : {};
}

function normalizeSequence(value, fallback = Date.now()) {
  const num = Number(value);
  return Number.isFinite(num) && num >= 0 ? Math.floor(num) : fallback;
}

function nowIso() {
  return new Date().toISOString();
}

export function createEventId() {
  if (globalThis.crypto?.randomUUID) {
    return globalThis.crypto.randomUUID();
  }
  return '10000000-1000-4000-8000-100000000000'.replace(/[018]/g, (char) => {
    const random = Math.floor(Math.random() * 16);
    return (Number(char) ^ (random & (15 >> (Number(char) / 4)))).toString(16);
  });
}

export function createEventIdempotencyKey({
  taskId = '',
  pluginRunId = '',
  eventType = '',
  controlRequestId = '',
  sequence = 0,
} = {}) {
  return [
    normalizeText(taskId),
    normalizeText(pluginRunId),
    'event',
    normalizeText(eventType),
    normalizeText(controlRequestId) || normalizeSequence(sequence, 0),
  ].join(':');
}

export function createRecordIdempotencyKey({
  taskId = '',
  pluginRunId = '',
  recordType = '',
  externalRecordId = '',
  sequence = 0,
} = {}) {
  return [
    normalizeText(taskId),
    normalizeText(pluginRunId),
    'record',
    normalizeText(recordType),
    normalizeText(externalRecordId) || normalizeSequence(sequence, 0),
  ].join(':');
}

export function buildTaskEvent({
  taskId = '',
  pluginRunId = '',
  eventType = '',
  source = WORKBENCH_EVENT_SOURCE.PLUGIN,
  sequence = 0,
  payload = {},
  controlRequestId = '',
  occurredAt = '',
  idempotencyKey = '',
  eventId = '',
  attemptId = '',
  leaseId = '',
  stationId = '',
  accountId = '',
  platform = '',
} = {}) {
  const normalizedSequence = normalizeSequence(sequence, Date.now());
  const normalizedEventType = normalizeText(eventType);
  const event = {
    eventId: normalizeText(eventId) || createEventId(),
    taskId: normalizeText(taskId),
    idempotencyKey: normalizeText(idempotencyKey) || createEventIdempotencyKey({
      taskId,
      pluginRunId,
      eventType: normalizedEventType,
      controlRequestId,
      sequence: normalizedSequence,
    }),
    eventType: normalizedEventType,
    type: normalizedEventType,
    source: normalizeText(source) || WORKBENCH_EVENT_SOURCE.PLUGIN,
    occurredAt: normalizeText(occurredAt) || nowIso(),
    sequence: normalizedSequence,
    eventSeq: normalizedSequence,
    payload: normalizeObject(payload),
  };
  const normalizedAttemptId = normalizeText(attemptId);
  const normalizedLeaseId = normalizeText(leaseId);
  const normalizedStationId = normalizeText(stationId);
  const normalizedAccountId = normalizeText(accountId);
  const normalizedPlatform = normalizeText(platform);
  const normalizedControlRequestId = normalizeText(controlRequestId);
  if (normalizedControlRequestId) event.controlRequestId = normalizedControlRequestId;
  if (normalizedAttemptId) event.attemptId = normalizedAttemptId;
  if (normalizedLeaseId) event.leaseId = normalizedLeaseId;
  if (normalizedStationId) event.stationId = normalizedStationId;
  if (normalizedAccountId) event.accountId = normalizedAccountId;
  if (normalizedPlatform) event.platform = normalizedPlatform;
  return event;
}

export function buildTaskRecord({
  taskId = '',
  pluginRunId = '',
  recordType = '',
  externalRecordId = '',
  sequence = 0,
  payload = {},
  collectedAt = '',
  idempotencyKey = '',
} = {}) {
  const normalizedSequence = normalizeSequence(sequence, Date.now());
  const normalizedPayload = normalizeObject(payload);
  return {
    idempotencyKey: normalizeText(idempotencyKey) || createRecordIdempotencyKey({
      taskId,
      pluginRunId,
      recordType,
      externalRecordId,
      sequence: normalizedSequence,
    }),
    recordType: normalizeText(recordType),
    platform: normalizeText(normalizedPayload.platform),
    targetKey: normalizeText(normalizedPayload.targetKey),
    externalRecordId: normalizeText(externalRecordId),
    sequence: normalizedSequence,
    collectedAt: normalizeText(collectedAt) || nowIso(),
    observedAt: normalizeText(normalizedPayload.observedAt),
    payload: normalizedPayload,
  };
}

/**
 * @param {Record<string, any>} [options]
 */
export function buildIngestEnvelope({
  taskId = '',
  pluginRunId = '',
  executorInstanceId = '',
  cursor = '',
  events = [],
  records = [],
  snapshot = null,
  attemptId = '',
  leaseToken = '',
  leaseEpoch,
  pageFingerprint = null,
  executionContext = {},
} = {}) {
  const normalizedTaskId = normalizeText(taskId);
  const normalizedPluginRunId = normalizeText(pluginRunId);
  const normalizedExecutorInstanceId = normalizeText(executorInstanceId);
  const normalizedAttemptId = normalizeText(attemptId);
  const normalizedLeaseToken = normalizeText(leaseToken);
  const normalizedContext = normalizeObject(executionContext);
  const normalizedAccountId = normalizeText(normalizedContext.accountId);
  const normalizedPlatform = normalizeText(normalizedContext.platform);
  const enrichEvent = (event = {}) => {
    const normalizedEvent = normalizeObject(event);
    return {
      ...normalizedEvent,
      eventId: normalizeText(normalizedEvent.eventId) || createEventId(),
      taskId: normalizeText(normalizedEvent.taskId) || normalizedTaskId,
      type: normalizeText(normalizedEvent.type) || normalizeText(normalizedEvent.eventType),
      eventSeq: normalizeSequence(normalizedEvent.eventSeq ?? normalizedEvent.sequence, Date.now()),
      attemptId: normalizeText(normalizedEvent.attemptId) || normalizedAttemptId || undefined,
      leaseId: normalizeText(normalizedEvent.leaseId) || normalizedLeaseToken || undefined,
      stationId: normalizeText(normalizedEvent.stationId) || normalizedExecutorInstanceId || undefined,
      accountId: normalizeText(normalizedEvent.accountId) || normalizedAccountId || undefined,
      platform: normalizeText(normalizedEvent.platform) || normalizedPlatform || undefined,
    };
  };
  /** @type {Record<string, any>} */
  const envelope = {
    protocolVersion: WORKBENCH_PROTOCOL_VERSION,
    taskId: normalizedTaskId,
    pluginRunId: normalizedPluginRunId,
    executorInstanceId: normalizedExecutorInstanceId,
    cursor: normalizeText(cursor),
    events: Array.isArray(events) ? events.map(enrichEvent) : [],
    records: Array.isArray(records) ? records : [],
  };
  const numericLeaseEpoch = Number(leaseEpoch);
  if (normalizedAttemptId) envelope.attemptId = normalizedAttemptId;
  if (normalizedLeaseToken) envelope.leaseToken = normalizedLeaseToken;
  if (Number.isFinite(numericLeaseEpoch)) envelope.leaseEpoch = Math.floor(numericLeaseEpoch);
  if (pageFingerprint && typeof pageFingerprint === 'object' && !Array.isArray(pageFingerprint)) {
    envelope.pageFingerprint = pageFingerprint;
  }
  if (snapshot && typeof snapshot === 'object' && !Array.isArray(snapshot)) {
    envelope.snapshot = snapshot;
  }
  return envelope;
}
