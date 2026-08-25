export function createSensitiveActionSet(MSG = {}) {
  return new Set([
    MSG.REMOVE_ACCOUNT,
    MSG.CLEAR_PLUGIN_AUTHORIZATION,
    MSG.DELETE_NOTE,
    MSG.DELETE_COMMENT,
    MSG.DELETE_AUTHOR,
    MSG.CLEAR_ALL_NOTES,
    MSG.CLEAR_ALL_COMMENTS,
    MSG.CLEAR_ALL_AUTHORS,
    MSG.ADD_ACCOUNT,
    MSG.UPDATE_ACCOUNT,
    MSG.SAVE_FLYWHEEL_CONFIG,
    MSG.GET_PLATFORM_COOKIES,
    MSG.SYNC_TO_WORKBENCH,
  ].filter(Boolean));
}

export function authorizeBackgroundMessage({
  action = '',
  sender = {},
  runtimeId = '',
  sensitiveActions = new Set(),
} = {}) {
  if (!sensitiveActions.has(action)) return { allowed: true };
  if (sender?.id && sender.id === runtimeId) return { allowed: true };
  return { allowed: false, error: 'unauthorized_sender' };
}
