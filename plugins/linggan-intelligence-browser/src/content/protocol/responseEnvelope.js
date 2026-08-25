import { MSG } from '../../shared/constants.js';
import { normalizeCompatResponse } from '../../shared/responseEnvelope.js';

const CONTENT_ARRAY_ACTIONS = new Set([
  MSG.GET_ALL_NOTES,
  MSG.GET_ALL_COMMENTS,
  MSG.GET_ALL_AUTHORS,
]);

const CONTENT_DIRECT_DATA_ACTIONS = new Map([
  [MSG.GET_STATS, (result = {}) => ({
    notes: Number(result.notes || 0),
    comments: Number(result.comments || 0),
    authors: Number(result.authors || 0),
  })],
  [MSG.GET_PAGE_CONTEXT, (result = {}) => result?.context || null],
  [MSG.RUN_DATA_MAINTENANCE, (result = {}) => result?.stats || {}],
  [MSG.DOWNLOAD_NOTE_MEDIA, (result = {}) => result?.summary || {}],
]);

export function normalizeContentMessageResponse(action = '', result) {
  const normalizedAction = String(action || '').trim();
  if (CONTENT_ARRAY_ACTIONS.has(normalizedAction)) {
    return normalizeCompatResponse(result, {
      dataValue: Array.isArray(result) ? result : [],
    });
  }
  if (CONTENT_DIRECT_DATA_ACTIONS.has(normalizedAction)) {
    return normalizeCompatResponse(result, {
      dataValue: CONTENT_DIRECT_DATA_ACTIONS.get(normalizedAction)(result),
    });
  }
  return result;
}
