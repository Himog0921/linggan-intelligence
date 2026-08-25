import test from 'node:test';
import assert from 'node:assert/strict';

import {
  WORKBENCH_RECORD_TYPE,
  WORKBENCH_TASK_EVENT_TYPE,
} from '../src/workbench/protocol/schema.js';
import {
  buildRecordSchemaObservability,
  validateRecordPayload,
} from '../src/workbench/protocol/recordPayloadValidator.js';
import {
  validateTaskEvent,
  validateTaskRecord,
} from '../src/workbench/protocol/validator.js';

test('record payload validator accepts every supported extractor output shape', () => {
  const samples = [
    [WORKBENCH_RECORD_TYPE.NOTE, { noteId: 'n1', title: '标题' }],
    [WORKBENCH_RECORD_TYPE.COMMENT, { commentId: 'c1', noteId: 'n1', text: '评论' }],
    [WORKBENCH_RECORD_TYPE.AUTHOR, { userId: 'u1', nickname: '作者' }],
    [WORKBENCH_RECORD_TYPE.MEDIA, { sourceUrl: 'https://example.com/cover.webp' }],
  ];

  for (const [recordType, payload] of samples) {
    assert.deepEqual(validateRecordPayload(recordType, payload), { valid: true, errors: [] });
  }
});

test('record payload validator rejects unusable note payloads with health counters', () => {
  const validation = validateRecordPayload(WORKBENCH_RECORD_TYPE.NOTE, {
    likes: 100,
  });

  assert.equal(validation.valid, false);
  assert.equal(validation.errors[0].code, 'missing_note_identity');

  const observability = buildRecordSchemaObservability({
    recordType: WORKBENCH_RECORD_TYPE.NOTE,
    validation,
  });

  assert.equal(observability.recordType, WORKBENCH_RECORD_TYPE.NOTE);
  assert.equal(observability.schemaValidationAttemptCount, 1);
  assert.equal(observability.schemaValidationFailureCount, 1);
  assert.equal(observability.recordSchemaFailed, true);
  assert.equal(observability.invalidRecordField, 'payload');
});

test('validateTaskRecord applies payload schema rules after envelope checks', () => {
  const result = validateTaskRecord({
    idempotencyKey: 'task_1:run_1:record:comment:c1',
    taskId: 'task_1',
    pluginRunId: 'run_1',
    recordType: WORKBENCH_RECORD_TYPE.COMMENT,
    payload: {
      commentId: 'c1',
      text: '没有父级作品',
    },
  });

  assert.equal(result.valid, false);
  assert.equal(result.errors.some((error) => error.code === 'missing_comment_parent'), true);
});

test('validateTaskEvent accepts workbench capability mismatch events', () => {
  const result = validateTaskEvent({
    idempotencyKey: 'task_1:run_1:event:task.capability_mismatch:1',
    taskId: 'task_1',
    pluginRunId: 'run_1',
    eventType: WORKBENCH_TASK_EVENT_TYPE.TASK_CAPABILITY_MISMATCH,
    payload: {
      reasonCode: 'page_type_mismatch',
    },
  });

  assert.deepEqual(result, { valid: true, errors: [] });
});
