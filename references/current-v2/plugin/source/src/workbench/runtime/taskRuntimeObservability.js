function normalizeText(value = '') {
  const text = String(value || '').trim();
  return text.length > 160 ? `${text.slice(0, 157)}...` : text;
}

function normalizeObject(value) {
  return value && typeof value === 'object' && !Array.isArray(value) ? value : {};
}

function normalizeNumber(value) {
  const num = Number(value);
  return Number.isFinite(num) && num >= 0 ? num : undefined;
}

function normalizeBoolean(value) {
  return typeof value === 'boolean' ? value : undefined;
}

function firstText(...values) {
  for (const value of values) {
    const text = normalizeText(value);
    if (text) return text;
  }
  return '';
}

function firstNumber(...values) {
  for (const value of values) {
    const num = normalizeNumber(value);
    if (num !== undefined) return num;
  }
  return undefined;
}

function compactObject(value = {}) {
  return Object.fromEntries(
    Object.entries(value).filter(([, item]) => item !== undefined && item !== ''),
  );
}

function rate(failed, attempted) {
  const failedCount = normalizeNumber(failed);
  const attemptCount = normalizeNumber(attempted);
  if (failedCount === undefined || !attemptCount) return undefined;
  return Number((failedCount / attemptCount).toFixed(4));
}

export function normalizeRuntimeObservability(value = {}) {
  const input = normalizeObject(value);
  const parseAttemptCount = firstNumber(input.parseAttemptCount, input.domParseAttemptCount);
  const parseFailureCount = firstNumber(input.parseFailureCount, input.domParseFailureCount);
  const schemaValidationAttemptCount = firstNumber(input.schemaValidationAttemptCount);
  const schemaValidationFailureCount = firstNumber(input.schemaValidationFailureCount);
  return compactObject({
    operation: firstText(input.operation),
    taskType: firstText(input.taskType),
    recordType: firstText(input.recordType),
    taskStrategy: firstText(input.taskStrategy),
    source: firstText(input.source),
    status: firstText(input.status),
    stage: firstText(input.stage),
    failureStage: firstText(input.failureStage),
    failureCategory: firstText(input.failureCategory),
    reasonCode: firstText(input.reasonCode),
    errorCode: firstText(input.errorCode),
    durationMs: firstNumber(input.durationMs),
    parseAttemptCount,
    parseFailureCount,
    parseFailureRate: firstNumber(input.parseFailureRate, rate(parseFailureCount, parseAttemptCount)),
    schemaValidationAttemptCount,
    schemaValidationFailureCount,
    schemaValidationFailureRate: firstNumber(
      input.schemaValidationFailureRate,
      rate(schemaValidationFailureCount, schemaValidationAttemptCount),
    ),
    itemAttemptCount: firstNumber(input.itemAttemptCount),
    itemFailureCount: firstNumber(input.itemFailureCount),
    domParseFailed: normalizeBoolean(input.domParseFailed),
    recordSchemaFailed: normalizeBoolean(input.recordSchemaFailed),
    invalidRecordField: firstText(input.invalidRecordField),
    report: normalizeBoolean(input.report),
  });
}

function shouldReportRuntimeEvent(eventType = '', observability = {}) {
  const type = normalizeText(eventType);
  const status = normalizeText(observability.status).toLowerCase();
  if (observability.report === true) return true;
  if (['task.completed', 'task.succeeded', 'task.failed', 'task.stopped', 'task.released'].includes(type)) return true;
  if (['done', 'completed', 'failed', 'stopped', 'canceled', 'rejected'].includes(status)) return true;
  if (Number(observability.parseFailureCount || 0) > 0) return true;
  if (Number(observability.schemaValidationFailureCount || 0) > 0) return true;
  if (observability.domParseFailed === true) return true;
  if (observability.recordSchemaFailed === true) return true;
  return false;
}

/**
 * @param {{task?: Record<string, any>, payload?: Record<string, any>, eventType?: string, now?: number, report?: boolean}} [options]
 */
export function buildTaskRuntimeObservability({
  task = {},
  payload = {},
  eventType = '',
  now = Date.now(),
  report,
} = {}) {
  const safePayload = normalizeObject(payload);
  const existing = normalizeObject(safePayload.observability);
  const metrics = normalizeObject(safePayload.metrics);
  const diagnostic = normalizeObject(safePayload.diagnostic);
  const latestSummary = normalizeObject(safePayload.latestSummary || safePayload.resultSummary);
  const taskPayload = normalizeObject(task.payload);
  const nowMs = Number(now);
  const startMs = firstNumber(
    task.startedAtMs,
    task.dispatchedAtMs,
    task.startedAt,
    safePayload.startedAt,
    latestSummary.startedAt,
  );
  const durationMs = firstNumber(
    existing.durationMs,
    safePayload.durationMs,
    metrics.durationMs,
    startMs !== undefined && Number.isFinite(nowMs) ? Math.max(0, nowMs - startMs) : undefined,
  );
  const parseAttemptCount = firstNumber(
    existing.parseAttemptCount,
    existing.domParseAttemptCount,
    metrics.parseAttemptCount,
    metrics.domParseAttemptCount,
  );
  const parseFailureCount = firstNumber(
    existing.parseFailureCount,
    existing.domParseFailureCount,
    metrics.parseFailureCount,
    metrics.domParseFailureCount,
  );
  const schemaValidationAttemptCount = firstNumber(
    existing.schemaValidationAttemptCount,
    metrics.schemaValidationAttemptCount,
  );
  const schemaValidationFailureCount = firstNumber(
    existing.schemaValidationFailureCount,
    metrics.schemaValidationFailureCount,
  );
  const itemAttemptCount = firstNumber(
    existing.itemAttemptCount,
    latestSummary.itemsPlanned,
    metrics.itemsPlanned,
    metrics.total,
  );
  const itemFailureCount = firstNumber(
    existing.itemFailureCount,
    latestSummary.failedItems,
    metrics.failedItems,
    metrics.failed,
  );
  const base = normalizeRuntimeObservability({
    ...existing,
    operation: firstText(existing.operation, safePayload.taskType, task.taskType),
    taskType: firstText(existing.taskType, safePayload.taskType, task.taskType),
    recordType: firstText(existing.recordType, safePayload.recordType),
    taskStrategy: firstText(existing.taskStrategy, task.taskStrategy, taskPayload.taskStrategy),
    source: firstText(existing.source, task.source),
    status: firstText(existing.status, safePayload.status),
    stage: firstText(existing.stage, safePayload.stage, safePayload.phase, diagnostic.stage),
    failureStage: firstText(existing.failureStage, diagnostic.stage, safePayload.stage, safePayload.phase),
    failureCategory: firstText(existing.failureCategory, safePayload.failureCategory, diagnostic.failureCategory),
    reasonCode: firstText(existing.reasonCode, safePayload.reasonCode, diagnostic.reasonCode),
    errorCode: firstText(existing.errorCode, safePayload.errorCode, safePayload.reasonCode, diagnostic.reasonCode),
    durationMs,
    parseAttemptCount,
    parseFailureCount,
    schemaValidationAttemptCount,
    schemaValidationFailureCount,
    itemAttemptCount,
    itemFailureCount,
    domParseFailed: existing.domParseFailed,
    recordSchemaFailed: existing.recordSchemaFailed,
    invalidRecordField: existing.invalidRecordField,
  });

  return {
    ...base,
    report: report === undefined ? shouldReportRuntimeEvent(eventType, base) : Boolean(report),
  };
}

/**
 * @param {{task?: Record<string, any>, payload?: Record<string, any>, eventType?: string, now?: number, report?: boolean}} [options]
 */
export function attachTaskRuntimeObservability({
  task = {},
  payload = {},
  eventType = '',
  now = Date.now(),
  report,
} = {}) {
  const safePayload = normalizeObject(payload);
  return {
    ...safePayload,
    observability: buildTaskRuntimeObservability({
      task,
      payload: safePayload,
      eventType,
      now,
      report,
    }),
  };
}
