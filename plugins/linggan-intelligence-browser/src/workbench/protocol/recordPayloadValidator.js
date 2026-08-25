import { WORKBENCH_RECORD_TYPE } from './schema.js';

function normalizeText(value = '') {
  return String(value || '').trim();
}

function normalizeObject(value) {
  return value && typeof value === 'object' && !Array.isArray(value) ? value : {};
}

function createValidationError(field, code, message) {
  return { field, code, message };
}

function hasAnyText(payload = {}, fields = []) {
  return fields.some((field) => normalizeText(payload[field]));
}

function hasRecordBody(payload = {}, fields = []) {
  return fields.some((field) => {
    const value = payload[field];
    if (Array.isArray(value)) return value.length > 0;
    if (value && typeof value === 'object') return Object.keys(value).length > 0;
    return normalizeText(value);
  });
}

function validateNotePayload(payload = {}, errors = []) {
  if (!hasAnyText(payload, ['noteId', 'platformContentId', 'contentId', 'url', 'canonicalUrl', 'rawUrl'])) {
    errors.push(createValidationError(
      'payload',
      'missing_note_identity',
      'note payload must include a stable note id or URL',
    ));
  }
  if (!hasRecordBody(payload, [
    'title',
    'content',
    'desc',
    'bodyText',
    'cover',
    'coverImg',
    'coverUrl',
    'images',
    'imageCandidates',
    'videoUrl',
  ])) {
    errors.push(createValidationError(
      'payload',
      'missing_note_body',
      'note payload must include visible content or media',
    ));
  }
}

function validateCommentPayload(payload = {}, errors = []) {
  if (!hasAnyText(payload, ['commentId', 'id', 'contentId'])) {
    errors.push(createValidationError(
      'payload.commentId',
      'missing_comment_identity',
      'comment payload must include commentId, id, or contentId',
    ));
  }
  if (!hasAnyText(payload, ['noteId', 'platformContentId', 'videoId', 'awemeId', 'targetId'])) {
    errors.push(createValidationError(
      'payload.noteId',
      'missing_comment_parent',
      'comment payload must include the parent note or video id',
    ));
  }
  if (!hasAnyText(payload, ['text', 'content', 'commentText'])) {
    errors.push(createValidationError(
      'payload.text',
      'missing_comment_text',
      'comment payload must include comment text',
    ));
  }
}

function validateAuthorPayload(payload = {}, errors = []) {
  if (!hasAnyText(payload, [
    'authorId',
    'userId',
    'platformAuthorId',
    'authorPlatformId',
    'profileUrl',
    'homepageUrl',
  ])) {
    errors.push(createValidationError(
      'payload.authorId',
      'missing_author_identity',
      'author payload must include a stable author id or profile URL',
    ));
  }
}

function validateMediaPayload(payload = {}, errors = []) {
  if (!hasAnyText(payload, ['assetId', 'sourceUrl', 'publicUrl', 'localPath', 'url'])) {
    errors.push(createValidationError(
      'payload.sourceUrl',
      'missing_media_identity',
      'media payload must include a stable asset id, URL, or local path',
    ));
  }
}

export function validateRecordPayload(recordType = '', payload = {}) {
  const errors = [];
  const normalizedType = normalizeText(recordType);
  const normalizedPayload = normalizeObject(payload);

  if (!normalizedPayload || Object.keys(normalizedPayload).length === 0) {
    errors.push(createValidationError('payload', 'required', 'payload must be a non-empty object'));
  }

  if (!Object.values(WORKBENCH_RECORD_TYPE).includes(normalizedType)) {
    errors.push(createValidationError('recordType', 'invalid', `Invalid recordType: ${normalizedType}`));
    return { valid: false, errors };
  }

  if (errors.length) return { valid: false, errors };

  if (normalizedType === WORKBENCH_RECORD_TYPE.NOTE) validateNotePayload(normalizedPayload, errors);
  if (normalizedType === WORKBENCH_RECORD_TYPE.COMMENT) validateCommentPayload(normalizedPayload, errors);
  if (normalizedType === WORKBENCH_RECORD_TYPE.AUTHOR) validateAuthorPayload(normalizedPayload, errors);
  if (normalizedType === WORKBENCH_RECORD_TYPE.MEDIA) validateMediaPayload(normalizedPayload, errors);

  return {
    valid: errors.length === 0,
    errors,
  };
}

export function buildRecordSchemaObservability({
  recordType = '',
  validation = { valid: true, errors: [] },
} = {}) {
  const errors = Array.isArray(validation?.errors) ? validation.errors : [];
  const firstError = errors[0] || {};
  return {
    recordType: normalizeText(recordType),
    schemaValidationAttemptCount: 1,
    schemaValidationFailureCount: validation?.valid === false ? 1 : 0,
    schemaValidationFailureRate: validation?.valid === false ? 1 : 0,
    recordSchemaFailed: validation?.valid === false,
    invalidRecordField: normalizeText(firstError.field),
    reasonCode: normalizeText(firstError.code),
  };
}

export function createRecordPayloadValidationError({
  recordType = '',
  validation = { valid: true, errors: [] },
} = {}) {
  const firstError = Array.isArray(validation.errors) ? validation.errors[0] : null;
  /** @type {Error & Record<string, any>} */
  const error = new Error(firstError?.message || 'record payload schema invalid');
  error.code = firstError?.code || 'record_payload_schema_invalid';
  error.reasonCode = error.code;
  error.retryable = false;
  error.validationErrors = Array.isArray(validation.errors) ? validation.errors : [];
  error.observability = buildRecordSchemaObservability({ recordType, validation });
  return error;
}
