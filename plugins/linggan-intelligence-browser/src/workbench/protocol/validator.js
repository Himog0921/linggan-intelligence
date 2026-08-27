import {
  REMOTE_TARGET_PAGE_TYPE,
  REMOTE_TASK_CONTROL_ACTION,
  WORKBENCH_RECORD_TYPE,
  WORKBENCH_TASK_EVENT_TYPE,
  WORKBENCH_MESSAGE_TYPE,
  WORKBENCH_PROTOCOL_VERSION,
  getSupportedRemoteTask,
} from './schema.js';
import { validateRecordPayload } from './recordPayloadValidator.js';

function createValidationError(field, code, message) {
  return { field, code, message };
}

function normalizeObject(value) {
  return value && typeof value === 'object' && !Array.isArray(value) ? value : {};
}

function validateProtocolVersion(protocolVersion, errors) {
  const normalizedVersion = String(protocolVersion || '').trim();
  if (!normalizedVersion) {
    errors.push(createValidationError('protocolVersion', 'required', 'protocolVersion is required'));
    return;
  }
  if (normalizedVersion !== WORKBENCH_PROTOCOL_VERSION) {
    errors.push(createValidationError('protocolVersion', 'unsupported', `Unsupported protocolVersion: ${normalizedVersion}`));
  }
}

function validateTaskType(taskType, errors) {
  const normalizedTaskType = String(taskType || '').trim();
  if (!normalizedTaskType) {
    errors.push(createValidationError('taskType', 'required', 'taskType is required'));
    return null;
  }
  const taskConfig = getSupportedRemoteTask(normalizedTaskType);
  if (!taskConfig) {
    errors.push(createValidationError('taskType', 'unsupported', `Unsupported taskType: ${normalizedTaskType}`));
    return null;
  }
  return taskConfig;
}

function validateTarget(target, errors, taskConfig = null) {
  const normalizedTarget = normalizeObject(target);
  const pageType = String(normalizedTarget.pageType || '').trim();
  const url = String(normalizedTarget.url || '').trim();
  if (!pageType) {
    errors.push(createValidationError('target.pageType', 'required', 'target.pageType is required'));
  }
  if (pageType && !Object.values(REMOTE_TARGET_PAGE_TYPE).includes(pageType)) {
    errors.push(createValidationError('target.pageType', 'invalid', `Invalid pageType: ${pageType}`));
  }
  if (!url) {
    errors.push(createValidationError('target.url', 'required', 'target.url is required'));
  }
  if (taskConfig && pageType && !taskConfig.targetPageTypes.includes(pageType)) {
    errors.push(createValidationError('target.pageType', 'mismatch', `Task does not support pageType: ${pageType}`));
  }
}

export function validateTaskEnvelope(envelope = {}) {
  const errors = [];
  const normalizedEnvelope = normalizeObject(envelope);
  const type = String(normalizedEnvelope.type || '').trim();
  if (type !== WORKBENCH_MESSAGE_TYPE.TASK_ENVELOPE) {
    errors.push(createValidationError('type', 'invalid', `Expected ${WORKBENCH_MESSAGE_TYPE.TASK_ENVELOPE}`));
  }

  validateProtocolVersion(normalizedEnvelope.protocolVersion, errors);

  const taskId = String(normalizedEnvelope.taskId || '').trim();
  if (!taskId) {
    errors.push(createValidationError('taskId', 'required', 'taskId is required'));
  }

  const taskConfig = validateTaskType(normalizedEnvelope.taskType, errors);
  const platform = String(normalizedEnvelope.platform || '').trim();
  if (!platform) {
    errors.push(createValidationError('platform', 'required', 'platform is required'));
  } else if (taskConfig && platform !== taskConfig.platform) {
    errors.push(createValidationError('platform', 'mismatch', `Task platform mismatch: ${platform}`));
  }

  validateTarget(normalizedEnvelope.target, errors, taskConfig);

  if (!normalizedEnvelope.payload || typeof normalizedEnvelope.payload !== 'object' || Array.isArray(normalizedEnvelope.payload)) {
    errors.push(createValidationError('payload', 'required', 'payload must be an object'));
  }

  return {
    valid: errors.length === 0,
    errors,
    taskConfig,
  };
}

export function validateCapabilityCheck(input = {}) {
  const errors = [];
  const normalizedInput = normalizeObject(input);
  const type = String(normalizedInput.type || '').trim();
  if (type !== WORKBENCH_MESSAGE_TYPE.CAPABILITY_CHECK) {
    errors.push(createValidationError('type', 'invalid', `Expected ${WORKBENCH_MESSAGE_TYPE.CAPABILITY_CHECK}`));
  }
  validateProtocolVersion(normalizedInput.protocolVersion, errors);
  const taskConfig = validateTaskType(normalizedInput.taskType, errors);
  const platform = String(normalizedInput.platform || '').trim();
  if (!platform) {
    errors.push(createValidationError('platform', 'required', 'platform is required'));
  } else if (taskConfig && platform !== taskConfig.platform) {
    errors.push(createValidationError('platform', 'mismatch', `Task platform mismatch: ${platform}`));
  }
  validateTarget(normalizedInput.target, errors, taskConfig);

  return {
    valid: errors.length === 0,
    errors,
    taskConfig,
  };
}

export function validateTaskControl(input = {}) {
  const errors = [];
  const normalizedInput = normalizeObject(input);
  const type = String(normalizedInput.type || '').trim();
  if (type !== WORKBENCH_MESSAGE_TYPE.TASK_CONTROL) {
    errors.push(createValidationError('type', 'invalid', `Expected ${WORKBENCH_MESSAGE_TYPE.TASK_CONTROL}`));
  }
  validateProtocolVersion(normalizedInput.protocolVersion, errors);

  const taskId = String(normalizedInput.taskId || '').trim();
  if (!taskId) {
    errors.push(createValidationError('taskId', 'required', 'taskId is required'));
  }

  validateTaskType(normalizedInput.taskType, errors);

  const action = String(normalizedInput.action || '').trim();
  if (!action) {
    errors.push(createValidationError('action', 'required', 'action is required'));
  } else if (!Object.values(REMOTE_TASK_CONTROL_ACTION).includes(action)) {
    errors.push(createValidationError('action', 'invalid', `Invalid control action: ${action}`));
  }

  return {
    valid: errors.length === 0,
    errors,
  };
}

export function validateTaskEvent(input = {}) {
  const errors = [];
  const event = normalizeObject(input);
  if (!String(event.idempotencyKey || '').trim()) {
    errors.push(createValidationError('idempotencyKey', 'required', 'idempotencyKey is required'));
  }
  if (!String(event.taskId || '').trim()) {
    errors.push(createValidationError('taskId', 'required', 'taskId is required'));
  }
  if (!String(event.pluginRunId || '').trim()) {
    errors.push(createValidationError('pluginRunId', 'required', 'pluginRunId is required'));
  }
  const eventType = String(event.eventType || '').trim();
  if (!eventType) {
    errors.push(createValidationError('eventType', 'required', 'eventType is required'));
  } else if (!Object.values(WORKBENCH_TASK_EVENT_TYPE).includes(eventType)) {
    errors.push(createValidationError('eventType', 'invalid', `Invalid eventType: ${eventType}`));
  }
  if (!event.payload || typeof event.payload !== 'object' || Array.isArray(event.payload)) {
    errors.push(createValidationError('payload', 'required', 'payload must be an object'));
  }
  return { valid: errors.length === 0, errors };
}

export function validateTaskRecord(input = {}) {
  const errors = [];
  const record = normalizeObject(input);
  if (!String(record.idempotencyKey || '').trim()) {
    errors.push(createValidationError('idempotencyKey', 'required', 'idempotencyKey is required'));
  }
  if (!String(record.taskId || '').trim()) {
    errors.push(createValidationError('taskId', 'required', 'taskId is required'));
  }
  if (!String(record.pluginRunId || '').trim()) {
    errors.push(createValidationError('pluginRunId', 'required', 'pluginRunId is required'));
  }
  const recordType = String(record.recordType || '').trim();
  if (!recordType) {
    errors.push(createValidationError('recordType', 'required', 'recordType is required'));
  } else if (!Object.values(WORKBENCH_RECORD_TYPE).includes(recordType)) {
    errors.push(createValidationError('recordType', 'invalid', `Invalid recordType: ${recordType}`));
  }
  if (!record.payload || typeof record.payload !== 'object' || Array.isArray(record.payload)) {
    errors.push(createValidationError('payload', 'required', 'payload must be an object'));
  } else if (recordType && Object.values(WORKBENCH_RECORD_TYPE).includes(recordType)) {
    errors.push(...validateRecordPayload(recordType, record.payload).errors);
  }
  return { valid: errors.length === 0, errors };
}
